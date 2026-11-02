# Paths through the books

The four books are [The Coax Programming Language](../../coax/book/README.md),
[Psionic: ML From Scratch](../../psionic/book/README.md),
[Building Aethyr, World Engine](../../aethyr/book/README.md) and
[Nostr From Scratch](../../nostr/book/README.md). Each route below
lists what you need and the chapters to read, in order, across the books.
Chapter numbers are each book's own.

Every command on this page runs from the root of the openagents repository.
The original command observations are from 2026-10-31 on a Mac with an Apple
M5 Max, 128 GB of memory and macOS 26.4, with compiler `coax0 0.45.0`, except
where a step names later evidence. The 2026-11-01 reconciliation updates the
reading routes from the current chapter registries; it does not rerun these
commands or qualify a later compiler or another host. Times below are retained
observations, not current performance guarantees. Retain a passing result only
while its source, compiler and options still match; rerun the affected examples
when that closure changes.

A chapter marked *(design draft)* or *(planned)* describes work that doesn't
run yet. The [chapter registries](../../coax/book/chapters.json) of the
[four](../../psionic/book/chapters.json)
[books](../../aethyr/book/chapters.json)
([Nostr](../../nostr/book/chapters.json)) record every chapter's status.
An implemented chapter qualifies its named profile, not every capability in
its title. See [completion milestones](completion.md) for the remaining
integration, quality, target and adoption gates.

## Write current Coax

| What you need | |
| --- | --- |
| Revision | `main` with compiler `coax0 0.45.0` or later. `coax/bin/coax version` prints it. |
| Hardware | A Mac with Apple silicon. The compiler needs a C and C++20 compiler (Xcode's command line tools) and Python 3. |
| Downloads | None. |
| Time | `coax/build.sh` takes about 10 seconds when it reuses a cached build, and about 12 minutes the first time on the tested Mac. |

First successful command:

```shell
coax/build.sh
coax/bin/coax run coax/book/examples/ch01_hello_name.coax
```

It prints `Hello, Coax!`.

Chapters, all in the Coax book:

1. Chapters 1–4: getting started, a first project, common concepts and
   reading errors.
2. Chapters 5–12: structs and enums, memory (partly implemented), errors,
   generics (partly implemented), compile time, larger programs, checks, and
   debugging.
3. Chapters 13 (partly implemented), 16 (partly implemented) and 18: data
   layout, determinism and the protocol boundary.
4. Chapters 20 and 24: interop with C and Swift, and a complete boundary.
5. Chapter 29's section "Points and vectors in a frame (runs today)". The
   rest of chapter 29 is planned.

To check your copy of the book's examples:

```shell
coax/book/check-examples.sh
```

It ends with `all book examples pass` and takes about 2 minutes on the
tested Mac.

## Build a playable realm

| What you need | |
| --- | --- |
| Revision | `main`, qualified with compiler `coax0 0.46.0`. |
| Hardware | A Mac with Apple silicon and Chrome, or another browser with WebGPU. |
| Downloads | LLVM for the WebAssembly build: `brew install llvm lld wasi-libc wasi-runtimes`. |
| Time | The dev server's first build takes about 2 minutes. |

First successful command, which runs the Aethyr book's examples in debug and
release:

```shell
coax/build.sh
python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax
```

It ends with `book execution: 38 runs match output and exit status`. The
2026-11-01 chapter 9 qualification uses `coax0 0.46.0`: 16 changed-closure
runs pass freshly, and 22 identical runs retain their passing evidence from
`bd73544a7e`. The [record](../../aethyr/tests/book-chapter9-qualification.json)
binds the compiler's source inventories, fixtures and run options. The new
example calculates an overridden run and jump, shows a carried fixed step,
and refuses zero gravity without losing the queued jump. Other commands on
this page retain their original qualification above.

To play a realm in the browser:

```shell
coax/web/serve.sh apps/openagents-web
```

When the server prints `coax serve: http://127.0.0.1:8800/`, open
`http://127.0.0.1:8800/verse`, choose **ENTER** to play as a guest, and the
Haunted House opens. The dev server rewrites generated content
files under `apps/openagents-web/`, so don't commit them by accident.

Chapters, in order:

1. Coax chapters 1–11, then 17 (GPU programs, partly implemented) and 19
   (builds and hot reload, partly implemented).
2. Aethyr chapters 1–5: the fixed step, spaces and units (native f32 spatial
   boundaries), defaults, typed input and replay.
3. Aethyr chapters 6–10: entities, spatial queries, ground/collision, walking
   and camera composition. These are implemented within their stated profiles;
   they do not supply a general rigid-body/contact solver.
4. Aethyr chapters 65–67: offline synthesized sound, sequencing, mixing and
   WAV output. Live playback and spatial/voice integration in 68–69 remain
   design drafts.
5. Aethyr chapters 11–15 (design drafts) and 16 (partly implemented): shared
   rendering, materials, light, figures, HUD and asset budgets. Existing realm
   shaders already emit from Coax; consolidating their shared renderer and
   independent references remains.
6. Aethyr chapters 56 and 64 (design drafts): the complete Grid capstone and
   whole-realm migration. Existing use of shared clocks, walking and queries
   is adoption of those components, not completion of either capstone.

## Run an original model

| What you need | |
| --- | --- |
| Revision | `main` with compiler `coax0 0.45.0` or later. |
| Hardware | The CPU path. The slice check reads an 18.8 GB file. A full decision needs about 18 GiB of memory. |
| Downloads | The four original Elephant files, 18.8 GB in all, from revision `b3f8a93462fb1051a9de031e94a7ff380e254072` of `baselabs/elephant-1` (see [the original bundle record](../../psionic/docs/original-bundles.md)). |

Download the files into one directory:

```shell
mkdir -p ~/models/elephant-1
for f in config.json model.safetensors tokenizer.json tokenizer_config.json; do
  curl -fL -o ~/models/elephant-1/$f \
    https://huggingface.co/baselabs/elephant-1/resolve/b3f8a93462fb1051a9de031e94a7ff380e254072/$f
done
```

For this page, each URL was checked to answer `200`; the files were not
downloaded again. The checks below authenticate every byte range they read
against the pinned digests.

First successful command, which needs no download:

```shell
coax/bin/coax run psionic/examples/elephant-target.coax
```

It prints the model's pinned shape, starting with `model: baselabs/elephant-1`.
Then check the original weights against recorded references, in about
11 seconds:

```shell
psionic/tests/check_elephant_slices.sh ~/models/elephant-1
```

It ends with `Elephant original-weight slices match the recorded CPU
references`. A whole decision on the CPU,
`psionic/tests/run_elephant.sh ~/models/elephant-1`, takes about 30 minutes;
it wasn't rerun for this page. Chapter 14 has its record.

Chapters, in order:

1. Coax chapters 1–4.
2. Psionic chapters 1–4: learning a number, shapes, views and matrices.
3. Psionic chapters 7 and 9–13: original weights, text requests, attention,
   the hybrid model, the decision mask and the options.
4. Psionic chapter 14: Elephant, end to end.
5. Psionic chapters 16 and 17: whether the probabilities are good, and how
   to qualify the result.

## Train a small model

| What you need | |
| --- | --- |
| Revision | `main` with compiler `coax0 0.45.0` or later. |
| Hardware | The CPU path. No GPU. |
| Downloads | None. |

First successful command:

```shell
coax/bin/coax run psionic/examples/learn-number.coax
```

It ends with `held-out: x = 3, prediction = 5.999999, target = 6`. Each
later chapter has its own example, which runs in a few seconds:

```shell
coax/bin/coax run psionic/examples/deadline-predictor.coax
coax/bin/coax run psionic/examples/gradient-tape.coax
coax/bin/coax run psionic/examples/byte-lm.coax
coax/bin/coax run psionic/examples/causal-decoder.coax
coax/bin/coax run psionic/examples/resume.coax
coax/bin/coax run psionic/examples/adamw.coax
```

Chapters, in order:

1. Coax chapters 1–4 and 11 (writing checks).
2. Psionic chapters 1–4.
3. Psionic chapters 103 and 104: learning with arrays, then gradients as a
   program.
4. Psionic chapters 112 and 113: a byte-level language model, then a
   differentiated decoder.
5. Psionic chapters 122 and 124: checkpoints that resume exactly, and AdamW
   with explicit precision.
6. Psionic chapter 115: the implemented end-to-end tiny-decoder CPU run.
   Chapters 114 and 116 are partly implemented: compiled backward and
   memory-budgeted training retain their named CPU profiles and open work.
7. Psionic chapters 123 and 125–128: learned tokenization, nanochat forward
   and backward, then the partly implemented complete CPU pretraining run.
   This is a small measured run, not a nanochat GPU speedrun reproduction.
8. Psionic chapters 129–131 are partly implemented: actual GPU decoder
   training, profiling and eager/compiled comparisons. Efficient cooperative
   kernels, mixed precision and the larger training acceptance projects remain.

## Sign and verify a Nostr event

| What you need | |
| --- | --- |
| Revision | `main` with compiler `coax0 0.45.0` or later. |
| Hardware | Any Mac that builds the compiler. |
| Downloads | None. The BIP-340 vectors are pinned in `coax/tests/vectors/`. |
| Time | About 15 seconds after the compiler is built. |

First successful command, which checks BIP-340 verification against the 19
official vectors:

```shell
coax/build.sh
coax/bin/coax test coax/lib/crypto/schnorr.coax
```

It ends with `127 checks, 127 passed, 0 failed`. Then sign and verify whole
events, including one signed independently in Python and matched byte for
byte:

```shell
coax/bin/coax test coax/lib/nostr/event.coax
coax/bin/coax test coax/lib/nostr/event_sign.coax --run-steps 250000000
```

They end with `183 checks, 183 passed, 0 failed` and
`332 checks, 332 passed, 0 failed`.

Chapters, in order:

1. Coax chapters 1–4, 11 (writing checks) and 18 (the protocol boundary).
2. Nostr chapters 1–4: bytes and hex, big integers in
   limbs, constant-time rules and SHA-256.
3. Nostr chapters 7–13: the field, scalars, the group
   law, scalar multiplication, verification, keys and signing.
4. Nostr chapters 17 and 21: the exact JSON an event ID
   hashes, then events, IDs and signatures.
5. Nostr chapters 22–24: kinds and tags, filters and
   relay messages, before a client (Part VIII) or a relay (Part IX).

These chapters are implemented within their declared profiles. Native/Wasm
qualification varies by chapter; a native result does not qualify a browser
secret-handling path. Chapters 103 and 104 remain partly implemented: production
relay migration/rollback and treasury signing/recovery have separate gates.
