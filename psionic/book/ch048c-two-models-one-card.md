# 48c. Two models, one card

[Chapter 48b](ch048b-serve-responses.md) serves gpt-oss-20b, which writes.
[Chapter 16](ch016-serve-systemone.md) serves Elephant, which decides: a typed
question in, probabilities out. A useful program wants both, one after the
other, each reading what the last one said. This chapter puts both servers on
one machine with one RTX 4080 and walks through a client that chains them.
Nothing here is new arithmetic; it is the order of operations and the numbers
that make it fit.

## The budget

The card has 16 GB. Its users when we did this:

| Process | Device memory |
| --- | --- |
| gpt-oss-20b, all original MXFP4/BF16 weights resident (chapter 45a) | 13,874 MiB |
| the desktop and headless browsers | about 250 MiB |
| left | about 1,400 MiB |

Elephant's device weights are 15.9 GB, more than the whole card. They fit
because of [chapter 30a](ch030a-share-the-card.md): the plan keeps what fits
and streams every other matrix from pinned host memory for each decision.
The smallest placement is the workspace, the f32 vectors and two 96 MiB
copy slots: 283 MiB for an 86-token decision and 676 MiB for 512 tokens.

gpt-oss cannot do the same. Its plan keeps every weight resident and refuses
to start if the card lacks the 13.8 GB. So the order matters: gpt-oss first,
then Elephant into what is left. If Elephant must start first, give it a cap
with `--device-mib` so the room gpt-oss needs stays free.

## Step 1: gpt-oss

```shell
export PSIONIC_API_KEY="$(openssl rand -hex 24)"
psionic/serve-gptoss.sh "$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee" 8080
```

It authenticates and uploads its weights once, then listens on
`127.0.0.1:8080`. `curl http://127.0.0.1:8080/healthz` answers
`{"status":"ok"}` when it is ready. Every other request needs the key.

## Step 2: Elephant beside it

Build the `/v1/systemone` server with the CUDA engine and start it on another
port:

```shell
psionic/platform/serve-build.sh build/serve release plain gpu
build/serve/psionic-serve --elephant-gpu /path/to/elephant-1 \
    --table psionic/fixtures/text/ch05/qwen35.pst --port 8797 --capacity 512
```

The first start hashes the 18.8 GB checkpoint and keeps a verified-file
record ([chapter 15's first part](../../docs/verification/psionic-ch15-verified-files-2026-10-29.md));
later starts of the same unchanged file skip the hash. The log of a later
start:

```jai
psionic-serve: authenticating elephant-1 (a verified-file record names this checkpoint; its hash is skipped)
psionic-serve: authenticated in 250 ms
psionic-serve: placed 0.33 GiB resident and 14.45 GiB pinned for streaming in 17057 ms (plan budget 1.01 GiB)
psionic-serve: elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba on http://127.0.0.1:8797/v1/systemone
```

The plan budget is what the card had free less a 256 MiB reserve. With
`--device-mib 800` it would be 800 MiB however much is free.

## Step 3: check the card

```shell
nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader
```

With both servers ready, gpt-oss held 13,874 MiB, Elephant 1,140 MiB, and
249 MiB stayed free. Elephant's host side holds 14.45 GiB of pinned memory
for the weights it streams, so the machine needs that much RAM to spare.

## Step 4: call them in turn

[`psionic/examples/clients/issue_files.py`](../examples/clients/issue_files.py)
asks which files of a repository go with an open GitHub issue. Each step reads
the previous step's answer:

1. `gh` reads the issue.
2. gpt-oss summarizes it and names search terms, as JSON.
3. `git grep` turns the terms into candidate files.
4. Elephant answers one choice question per file, all in one request: is this
   file one that must be read or changed to resolve the issue? Each answer
   carries `p(yes)`.
5. gpt-oss reads the ranking and writes the report.

```shell
python3 psionic/examples/clients/issue_files.py 12608
```

The client takes the gpt-oss key from `PSIONIC_API_KEY`, or, for a server on
this machine run by the same user, from that server's own environment; it
never prints it. For issue 12608 (fast Elephant on Apple GPUs):

```jai
== 2. gpt-oss summary and terms (5.6 s)
terms: dw_project, threadgroup, simdgroup_matrix, simd_shuffle, __shared__, __syncthreads, prefix reuse, checkpoint hash, coax compute subset, apple matrix units

== 4. Elephant decisions (8.5 s for 10 files)
p(yes) 0.936  coax/lib/compiler/compute.coax
p(yes) 0.885  coax/tests/metal_compute.py
p(yes) 0.809  coax/src/emit_c.cpp
p(yes) 0.806  psionic/book/ch031-elephant-on-metal.md
p(yes) 0.710  psionic/book/ch026-a-coax-compute-subset.md
p(yes) 0.688  docs/verification/psionic-ch31-2026-10-29.md
p(yes) 0.456  coax/tests/cuda.py
p(yes) 0.454  INVARIANTS.md
p(yes) 0.338  psionic/book/PLAN.md
p(yes) 0.210  coax/README.md

== 5. gpt-oss report (9.4 s)
```

The report started with `compute.coax`, `emit_c.cpp` and the Metal tests and
named the CUDA tests, `INVARIANTS.md`, the plan and the compiler README as
false matches. The whole chain took about 24 s.

The division of labor is the point. gpt-oss is good at reading loose text
and writing; its output is text and has to be parsed (the client takes the
first JSON object and falls back to the issue's own words). Elephant's output
is a probability per typed question, so the ranking needs no parsing and can
be thresholded or audited, and ten questions cost one request.

## What it costs

- Elephant streams about 15.5 GB per decision instead of reading it from
  device memory: about 0.68 s per decision against 147 ms with the card to
  itself, and its answers are the same to every printed digit (chapter 30a).
  A request with ten questions runs ten decisions in order, about 0.85 s each
  with the longer prompts here.
- gpt-oss is unchanged while Elephant is idle. When both run at the same
  moment they share the PCIe link and the compute units; that has not been
  measured.
- Elephant's placement is decided once, at start. If gpt-oss stops, Elephant
  keeps its small placement until it is restarted.

## Limits

Two processes, one card, no scheduler: neither server knows about the other,
and the order of starts is the only coordination. A card scheduler that hands
memory to whichever model is busy, batching several decisions into one pass,
and quantized weights (chapter 34) that halve what Elephant streams are later
work.
