# 70. Replay the device program

Chapter 66 keeps a complete decoder on the GPU, but its host still submits
hundreds of kernels for each token. A CUDA graph can retain that ordered work
and submit it through one driver call. The model's arithmetic need not change.

The [Coax replay policy](../lib/gptoss/device-replay.coax) decides when replay
is allowed and which admitted descriptors changed. The
[CUDA adapter](../platform/cuda-host.c) owns graph objects, driver calls and
resource destruction. The [session adapter](../platform/gptoss-device.c)
connects them. Every kernel still comes from Coax.

## Retain order and patch the changing arguments

We keep using `gd_program` for every step. It admits token, absolute position,
output limit, work counts and typed views before any work is submitted. The
first synchronous decode creates a graph whose nodes have the same insertion
order as that program. Every node depends on its predecessor. We retain the
original graph and its node handles alongside the executable graph; no code
assumes CUDA enumerates nodes in insertion order. Parameter updates use
[NVIDIA's driver graph API](https://docs.nvidia.com/cuda/cuda-driver-api/group__CUDA__GRAPH.html)
with the retained original node handles.

On the next step, Coax compares semantic fields of the last installed program
with the newly admitted program. It compares scalar values, work counts,
block sizes and each view's buffer, offset, element count and element size.
Unused fields and native struct padding do not affect the decision.

Token and position change the beginning node. Rotary and attention nodes
change with position and visible history. The output limit can change the
selection node. Most coefficient bindings and workspace views remain fixed.
Only the listed nodes receive new driver parameters. For a two-layer owned
model, moving from position 127 to 128 while changing the token and output
limit updates ten nodes; the wrapped layer's visible extent stays bounded.

A different kernel sequence or argument count rebuilds the graph. In
particular, toggling token selection changes its final two nodes. Malformed
descriptor spans refuse, and an insufficient update-list capacity leaves that
list untouched. These checks supplement the original program admission.

## Complete a step before updating its recording

Replay requires a synchronous GPU step, exclusive session access and no
per-kernel event instrumentation. Tiny CPU emulation and asynchronous stream
submission retain their existing paths. Instrumented profiling also uses
ordinary launches so its per-kernel events retain their meaning.

After a replay, the control download is enqueued on the same stream. Its wait
completes both the kernels and the download; an extra wait before the download
is unnecessary. The next synchronous call can safely update the recording.

The graph does not bypass numeric flags or publication. A nonfinite result
still leaves public cache/logits unchanged. A driver error poisons the session.
Cancellation remains between complete decode steps and between prefill chunks.
No speculative work runs beyond a step's established publication boundary.
Reset clears request state while retaining weights, allocations and the graph.
Closing waits for outstanding work, destroys the graph, then releases its
buffers, module and stream.

## Check behavior, then measure

The [focused GPU check](../tests/gptoss_replay.c) compares ordinary launches
and graph replay byte for byte over the complete public cache, logits, traces,
expert IDs, mixture weights and control words. It crosses a sliding window,
resets requests, changes selection topology and interleaves an asynchronous
step. The existing [tiny decoder check](../tests/gptoss_device.c) also exercises
late arithmetic failure and cancellation boundaries with the replay backend.

```shell
coax/bin/coax test psionic/lib/gptoss/device-replay.coax
PSIONIC_CUDA=1 psionic/tests/check_gptoss_device.sh
```

The [paired benchmark](../tests/gptoss_speed.c), built with
`PSIONIC_DEVICE_TEST`, accepts `--host-replay` after its four path arguments.
Both variants execute the identical generated module and Coax program. The
test's private switch selects ordinary launches or replay; production exposes
no backend switch. Each variant warms up and then runs five paired trials,
including actual greedy choices and complete-state parity. Core timing does
not include HTTP or tokenization.

The [third-pass record](../../docs/verification/psionic-decode-pass3-2026-10-29.md)
retains paired measurements, actual Responses timing and build identities.
The [performance notebook](../docs/serving/performance.md) records rejected
experiments and the remaining bottlenecks. Graph replay removes submission
work; it does not reduce coefficient bytes or accelerate prefill's matrix work.
