# 45c. Prefill together

A prompt is already known. Running a complete vocabulary head and a separate
matrix-vector product for every prompt token wastes that knowledge. Prefill
can process a bounded group of tokens through each layer, reusing coefficients
across those tokens, then read the vocabulary only for the last token.

## Separate two measurements

Time to first token includes prompt processing. Decode speed measures subsequent
steps once the prompt's keys and values exist. A fast decoder can still have a
slow prompt path. Always state input/output lengths and report both times.
The original serial path on the 4080 demonstrated that distinction: roughly
45 forced decode tokens/s at 1,024 positions, but about 19 seconds of serial
prefill for a repeated-token workload. That exploratory corpus differs from the
diverse fixed-token benchmark used for the batched path.

[The prefill planner](../lib/gptoss/device-prefill.coax) validates every token
before upload and divides the prompt into chunks of at most 128. It emits a
separate launch program per chunk. The host uploads the prompt once, launches
Coax's programs and reads a small control record after each chunk.

Within a layer, each token retains its own normalization, Q/K/V, route and
residual stream. A four-row/eight-token tile reuses coefficients and inputs. Tensor-relative
addresses use u32 arithmetic after the Coax plan bounds every shape, buffer
and work-item count; admitted addresses never wrap. View bounds checks remain
in the generated code. The first checked signed-64 address layout was too
slow, and a scalar-only tile regressed. Both were measured before tuning the
address path. No arithmetic is supplied by a host library.

## Group selected tokens by expert

After routing, GPU kernels construct a stable list of token/slot pairs for
each expert. Prefix counts turn these lists into eight-token tiles. Each matrix
product uses only the selected experts' packed original rows. A ragged tile
reuses its last valid input for inactive accumulator lanes, then publishes
only valid lanes. One output has exactly one writer. Expert IDs, mixture
coefficients and intermediate activations remain on the device.

The eight accumulators use the single-token lane partition and reduction
order. Owned checks compare batch splits 1/3, 2/2, 3/1 and 4 against the dense
reference and require identical complete cache bytes to serial execution.
A distinct test checks the ragged expert grouping and hand-computed products.

## Causal attention across a chunk

For query position `p`, attention still sees only allowed positions through
`p`, including earlier tokens in the current chunk. Old positions come from
the cache shadow; new positions come from this layer's batched pending K/V.
The sliding layer's first visible position is `max(0, p+1-window)`.
The sink retains its probability mass. Future chunk tokens never participate.

A chunk begins by copying the public cache into a private shadow. Each layer
writes only its completed chunk entries to that shadow. The ring writer keeps
only the newest entries when the chunk is longer than its window, assigning
one writer to each retained cache slot. Only the last token's layer diagnostics
and vocabulary head are exposed.

A flag reduction follows each reused flag buffer. A sticky numerical refusal
prevents final publication. After the final head succeeds, publication copies
the complete shadow and outputs together, then advances by the chunk length.
The transaction boundary is one chunk: earlier successful chunks stay committed
if a later chunk fails. Request-level rollback remains a caller operation.
The late-head failure test checks all public cache bytes and logits.

## Measure the result, keep the next gate honest

```shell
PSIONIC_CUDA=1 \
PSIONIC_GPTOSS_ORIGINAL="$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee" \
PSIONIC_DEVICE_RECORD=/tmp/gptoss-device-record \
COAX=coax/bin/coax psionic/tests/check_gptoss_device.sh
```

This is explicit original-GPU work. Default checks stay small. The benchmark
warms up, then repeats a fixed diverse 1,024-token input / 128 forced-token
continuation five times. It measures the core execution path; forced tokens
are not sampled chat output or an answer-quality result. The independent
original-vector qualification and complete generated Harmony turn run first.

The [record](../../docs/verification/psionic-gptoss-device-2026-10-29.md) contains
actual results, build identity and unmet goals. The [plan](PLAN.md) keeps the
2-second prefill goal, a packaged-reference comparison, continuous batching,
120b device execution and fast speculation as explicit gates. Tiling alone
does not establish tensor-core utilization or production serving throughput.
