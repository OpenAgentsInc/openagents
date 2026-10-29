# Psionic chapter 16: `/v1/systemone` on the RTX 4080, 2026-10-29

Verified on `coderos-4080` (Linux x86-64, Intel Core i7-14700K, NVIDIA
GeForce RTX 4080 16 GB, CUDA 13.2 NVRTC) with coax0 built from this tree.
The device held only desktop and browser processes (under 1 GB) during the
measurements. An earlier attempt found another agent's server holding 13.9 GB,
and the server refused placement with "the plan refused 128 tokens in
686096384 bytes: status 3".

## What was built

- `psionic/platform/elephant-gpu.{c,h}`: the reusable device engine.
  `eg_compile` opens the device and compiles project, layer and warp `.cu`
  (before authentication, so a missing driver is found first). `eg_place`
  places the authenticated session's weights by the Coax plan for the
  largest decision. `eg_decide` stages inputs, issues every operation and
  returns the logits, asking a stop callback at each layer start.
  `psionic/tests/elephant_device.c` (chapter 30) now opens through it and
  issues operations with `eg_issue`. Its profile, replay and qualify modes
  are unchanged.
- `psionic/lib/device/plan.coax` (`#authority`): `dp_replan` schedules a
  later decision on the opened placement (same streamed set, workspace large
  enough) and `dp_stop_point` marks the 33 layer and readout starts. 2,269
  mutants, 0 survivors.
- `psionic/lib/serve-run.coax`: `sv_stage` (prepare and fit, the
  `sv_answers` refusals) and `sv_read` (chapter 10's `ps_answer` readout
  over device logits), checked against `qr_read`.
- `psionic/platform/serve-host.c`: `--elephant-gpu DIR`. The decision runs in
  process, because a CUDA context does not survive fork. Deadline and
  disconnect are checked between layers, and a stopped decision publishes
  nothing. Also the chapter 16b protobuf hooks (`sp_bytes`, `sp_answer`;
  refusals stay JSON) for every engine. `serve-build.sh OUT MODE NATIVE gpu`
  builds it, and the unit is now `serve-proto.coax`.

## Results

Cold start (log lines): kernels compiled in 0.27 s, authentication 72.7 s,
placement 1.34 s (13.85 GiB resident, 0.94 GiB pinned for streaming, budget
14.19 GiB), ready in 76 s. A second start, with CPU checks running at the
same time, took 123 s (authentication 121 s).

Warm, chapter 10's sample (`psionic/fixtures/models/serve/elephant-request.json`),
one loopback connection per request, `curl` `time_total`:

| Run | JSON (ms) |
| --- | --- |
| session A | 155.4, 149.5, 149.6, 146.9, 147.6, 148.5, 147.0 |
| session B | 149.0, 147.6, 148.0, 147.2, 147.2 |

Twelve runs: 146.9 to 155.4 ms, median 147.8 ms. The served `latency_ms` was
146 to 155. The protobuf round trips (session B, Python `http.client`, request
built and response parsed with the official runtime through
`psionic/tests/serve_proto_oracle.py`) took 149.2, 146.6 and 147.4 ms and
decoded to the same values.

Every answer was `yes`, p_yes 0.757202773, p_no 0.242797227, which is chapter
30's device result. The difference from chapter 10's recorded CPU result
(0.757196285) is 6.5e-6, within the frozen 0.0005. Elephant was not run on the
CPU.

Cancellation: clients that hung up at 50 ms (twice) and 80 ms were logged
`cancelled, client closed` at 53 and 84 ms, and the next request answered the
same. With `--deadline-ms 60`, two requests were refused `524 timeout` at
64 ms. A wrong model was refused `403 not_admitted` in 0 ms without device
work.

The first build waited up to a second before publishing each answer: the
poll timeout for the deciding slot overrode the immediate wake-up. This was
fixed before the numbers above were taken.

## Checks run

- `coax test` on `plan.coax` (43 checks) and `serve-run.coax` (306 checks),
  `coax mutate psionic/lib/device/plan.coax`: 0 survivors.
- `psionic/tests/serve.sh` (debug and release, plain and ASan/UBSan, through
  `serve-proto.coax`): all native checks pass.
- `psionic/tests/check_serve_proto.sh` with `PROTO_PYTHON` set to the
  protobuf 5.29.3 venv.
- `serve-build.sh` with and without `gpu`; `elephant_device.c` compiles
  against `elephant-gpu`.
- `python3 -I psionic/tests/book.py`.

## Not done

- The chapter 30 harness was not rerun end to end after the refactor. The
  served path uses the same engine and plan and gives the same p_yes.
- Multi-question requests run one device decision per question in order,
  with no batching. Only the sample request was timed.
- The cold start is the CPU authentication, which the chapter 15 identity
  cache will shorten.
- The server checks a decision between layers, so cancelling takes up to one
  layer, about 5 ms.
