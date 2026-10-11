# Full text-029 precision controls

This is a fixed causal diagnostic, not a replacement acceptance oracle. The
unchanged f32 same-Q4 reference remains A. The previously measured all-top gate
remains unmet at 103 of 104 answers. Its 0.02 probability limit, all-top
requirement, and separate within-pipeline criteria do not change.

## Two controls

Run only text 029, with its exact 359 IDs and all three original questions. Do
not add variants, select a favorable checkpoint, tune a projection, or run the
40-request suite under a revised precision criterion.

- **B:** round every trunk `nn.Linear` weight and input through f16, then execute
  the unchanged official f32 operators. Convolution, norms, recurrent state,
  and residuals remain f32. This is the successful layer-0 WX boundary control
  applied uniformly to all 64 layers.
- **C:** apply the same projection boundaries as B, plus scaled rotary Q and
  K/V conversion through f16 and the reviewed original 32-key online attention
  calculation from `layer35_math.py`. Its unnormalized exponentials pass
  through f16, with the documented denominator reduction and rescaling.

Both controls retain the independent official chunk64 Delta operator. There
is no new recurrent variant: the layer-0 control found that switching only
the official Delta formulation had little effect there. That local result
does not prove reduction order immaterial at every layer.

Decode and round one layer at a time, then process B and C sequentially as
separate batch-size-one requests with no cache. Their hidden states remain
separate. Sharing immutable weights reduces repeated decoding; it does not
share request state or combine requests into a padded batch. Check parameter
bytes before and after the two forwards and storage/version identity after
each. Check each incoming hidden array byte for byte after its forward.

## Fixed predictions and interpretation

Layer 0 showed that both projection boundaries reduced the native discrepancy
about 11.2 times. Layer 35 showed amplification of incoming differences. If
the documented precision boundaries explain much of the accumulated native
trajectory, B or C should track native layer outputs more closely than saved
A, and their full-head probabilities should move toward native. The complete
trajectory and all three questions must be reported even if this prediction
fails or the disputed top does not change.

Compare B minus A, C minus A, C minus B, B minus native, and C minus native at
every layer, for all rows and the already identified question/option spans.
This separates propagation under projection rounding from the additional
attention boundaries. No distance threshold determines a winning variant.
Retain negative results. Even a matching diagnostic top would not satisfy the
original f32 runtime gate or prove every native operator correct.

The controls are not bitwise Metal emulators. CPU GEMM and RMSNorm reductions,
transcendental functions, fused multiply-add, state-output reduction, and the
native residual accumulator still differ. Native incoming embedding equality
is reconstructed from exact IDs, decoder evidence, and source; that input
was not independently saved before the archived native layer-0 capture.

## Identity and checks

Use only original GGUF SHA-256
`9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`,
the independently decoded CPU reference in `cpu-1`, and the archived Stage 6
native rows in `native-1`. The native binary SHA-256 is
`746950810194cbb9228ab6c49776beb5bbb45415b831e5c94efb821ad2dde566`.
Verify the historical GGUF file state, pinned official/importer source hashes,
the exact request and freshly encoded IDs/spans/options, saved-array hashes,
and the prior full-head observer equivalence proof. Keep original artifacts
and receipts unchanged.

Replay only A's saved layer-63 rows through the unchanged final RMSNorm and
official f32 head. Its final hidden bytes, lexical bytes, and logits must
reproduce the saved CPU reference exactly. No A trunk rerun is needed.
The first-layer B and C outputs must exactly match the saved layer-0 WX
control. B and C must remain bitwise equal through layers 0–2, before their
first full-attention difference. The previously reviewed attention fixtures
and small independent-f64 Delta fixtures run before the full trunk.

At the end, apply the same original output RMSNorm and unchanged official
f32 head separately to B and C. The output norm is applied once, to each
pre-normalized layer-63 output. Keep the original GGUF lexical rows; do not
round or replace head weights, lexical inputs, probabilities, or questions.
Verify all head inputs and weights remain unchanged. Compare all three sets
of logits, probabilities, and tops against A and both saved native modes.
Verify the saved native HTTP token, cache, artifact, and answer contracts.

Save all 64 pre-final-norm hidden arrays for each control, their full hashes
and distances, final normalized memory, exact token IDs, lexical rows, lexical
remapping, masks, original spans/options, and all head outputs. Raw arrays and
weights stay outside Git. Publish complete numerical receipts, source hashes,
and limits; split a large receipt losslessly if the publisher requires it.

## Proposed bounds and cost

Do not run until root reviews the sources, a peer clears them, and root assigns
a CPU workload window. The proposed guard uses memory 12 GiB, 24 GiB starting
available memory, a 12 GiB floor, two CPU threads, a 300-second deadline, disk
2 GiB, and a 25 GiB free-disk floor. It has a checked 2 GiB total output cap.
No GPU, build, quiet lease, full-model resident allocation, desktop app, or
system service is involved. Stop only the owned child group on failure;
preserve the result and do not retry or raise limits automatically.

The original one-pass CPU capture took 75.081 seconds: 43.672 seconds of layer
loading/decoding and 24.071 seconds of forward work. Sharing each loaded layer
gives roughly 92 seconds of that same basic work for two passes, before
attention controls, parameter hashes, comparisons, and output I/O. The
six-variant layer-0 control took 27.766 seconds and peaked at 3.885 GB.
Estimate 150–240 seconds for this full control; the unchanged 300-second
deadline decides completion. This is a correctness estimate, not a latency
benchmark. One f32 decoder layer is live at a time, with two 7.35 MB hidden
states and small comparison/head buffers. Keep the existing 12 GiB cap.

The 128 layer arrays contain 941,096,960 raw bytes. Final head inputs and
metadata keep expected output below 1.1 GB, within the 2 GiB reservation.
Weight bytes are not written. No source or result authorizes an additional
full case, new variant, or acceptance run.
