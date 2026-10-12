# Clef image decision parity

These six synthetic cases cover 14 questions: solid-color PNG and JPEG images,
odd-size PNG and WebP images with opposite left/right colors, and two-image
requests in both orders. Opposite-image pairs keep their text identical so a
decoder that ignores images cannot pass. The cases include noul, choice, and
score answers.

`../tools/image_parity.py reference` calls the released Cloudflare
`load_release_model` and `systemone` functions with complete local Hugging Face
weights. It uses eager attention and records dtype, device, dependency versions,
source and image hashes, complete responses, token counts, and wall time.
It also records raw option logits and input token IDs. It checks every shard
against the weight index before loading and refuses cases above its token bound.
It never downloads weights. CPU execution uses one or two numerical threads.

```sh
python ../tools/image_parity.py reference --checkpoint /path/to/clef-bf16 \
  --device cpu --dtype bfloat16 --threads 2 --output /path/to/reference.json
```

After that process exits, start the native server with its matching decoder,
head, and vision manifest, then run:

```sh
python ../tools/image_parity.py native --endpoint http://127.0.0.1:PORT \
  --reference /path/to/reference.json --output /path/to/native.json
```

The native check requires all 14 top answers to match both the reference and the
declared image contents, with identical input token counts. For score answers,
the top answer is the most probable level; the reported score remains the
probability-weighted value. Probability differences are reported without a
numerical-parity claim. Both processes write partial results after every case;
an interrupted or incomplete reference cannot pass the native check.

These fixtures define acceptance. They are not evidence that model execution
has passed; keep actual reference and native result files with the audit report.

For a shared Mac, wrap reference execution in `../tools/guarded_reference.py`
under memory and GPU leases. The GPU lease keeps native and reference model
loads separate, including a CPU reference. The wrapper checks available memory
before starting its child, samples only that child's process group every
0.5 seconds, and stops it if RSS, the available-memory margin, or the deadline
exceeds the configured limit. These are sampled limits, not OS-enforced memory
reservations. It never stops an existing application or system service.

Start with `--case solid_red`, then use `--resume` without `--case` after
reviewing the pilot. The runner saves each completed case with an atomic file
replacement. A partial suite has `completed: false`; native acceptance refuses
it. Resuming requires the same checkpoint, dependency versions, suite, and
execution settings. `--device mps --mps-memory-gib 60` sets a separate PyTorch
MPS allocation limit. Keep the default eager attention for the reference.
Resuming also checks the head and tokenizer configurations and compares saved
token IDs with fresh encoding. Response checks require confidence, choices,
and scores to agree with their probabilities within the reference's rounding.

The same reference runner accepts
`--text-suite ../e2e/requests.jsonl --case text-006`. This is the smallest of
the 40 text requests: 155 tokens under the official 27B tokenizer. The suite
has 104 questions and a maximum of 1,090 tokens. Native HTTP requests preserve
source number spellings, and the reference parses those bytes with Python's
original JSON semantics. `--resume` finishes the remaining text requests.
For this text suite, the receipt reports the original M1 numerical gate
separately: all top answers must match and the maximum probability difference
must be at most 0.02. Text acceptance also requires identical token counts.
Every question retains its decision type, both top answers, both top-two
probability margins, and maximum probability difference, including mismatches.

`../tools/native_image_parity.py` takes a JSON configuration with the binary,
GGUF, external BF16 head, vision manifest, reference files, and memory limits.
It records hashes for those artifacts, starts its own server on an assigned
loopback port, runs the comparisons, and closes its process group. Using the
external BF16 head is a separate comparison from using the head embedded in
the GGUF. Image acceptance also checks ten malformed or unsupported media
requests for their HTTP status and error code.

`image_parity.py self-test` checks missing, mistyped, cached, and nonfinite
responses without loading a model. `image_parity.py device-check --device mps`
compares small DeltaNet and BF16 vision-convolution operations with CPU outputs;
it requires a GPU lease and does not establish full-model parity. The prepared
reference environment uses PyTorch 2.9.1, Transformers 5.10.2, and
Accelerate 1.15.0.
