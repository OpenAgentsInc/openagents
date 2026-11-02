# Paths through the books

The four books with routes here are [The Coax Programming Language](../../coax/book/README.md),
[Psionic: ML From Scratch](../../psionic/book/README.md),
[Building Aethyr, World Engine](../../aethyr/book/README.md) and
[Nostr From Scratch](../../nostr/book/README.md). Chapter numbers below are
each book's own. Two more books have no runnable route yet:
[Coder: Coding Agents From Scratch](../../coder/book/README.md), whose harness
chapters moved from Part XXI of the Psionic book, and
[Bitcoin From Scratch](../../bitcoin/book/README.md), Nostr's companion, which
took over the cryptography chapters Nostr used (Nostr chapters 1–13, 15 and
18).

The [supported-route catalog](routes.json) declares four bounded CPU routes.
The [delivery process](DELIVERY.md) explains clean-checkout execution, selective
reuse and source-bound publication receipts.
Each names its prerequisites, commands, exact stdout fixture, expected exit,
source/evidence inputs and remaining limits. Existing examples make a route
available to run; a matching receipt establishes what actually passed for a
particular compiler, source closure, options and host. A matching transcript
is not whole-book completion, product adoption or deployment readiness.

Run these commands from the repository root with an existing compiler:

```shell
python3 scripts/book_routes.py --check
python3 scripts/book_routes.py --route coax-current-ffi --plan
python3 scripts/book_routes.py --route coax-current-ffi \
  --coax /absolute/path/to/coax --receipt /tmp/coax-current-ffi.json
```

`--check` validates metadata only. `--plan` discovers dependencies when needed and describes the selected work
without executing route examples (compiler front-end checks can run). `--route` with `--receipt` runs only the declared route steps and
records their actual producer identities and results. Supply `--previous FILE`
to offer an earlier receipt for reuse; a prior pass applies only when its full
required closure matches. The runner does not build a compiler, download data,
start a browser or contact a service. The Psionic route performs its declared
tiny synthetic training run; it does not retrain MNIST.

The terminal routes require Python 3, a native C compiler and the chosen Coax
executable; the C boundary also needs Bash. Retained observations use Apple
silicon/macOS. Another compiler or host requires its own matching evidence.
All four routes need no GPU, external data, network access or user secrets.
The route receipt, rather than an old compiler version or duration printed on
this page, records the executed profile. Every declared exit is zero and
stdout must match the linked fixture bytes. Standalone Coax examples run in
debug and release. The C workflow runs once and itself executes two C builds.

A chapter marked *(design draft)* or *(planned)* describes work that does not
run yet. The chapter registries for [Coax](../../coax/book/chapters.json),
[Psionic](../../psionic/book/chapters.json),
[Aethyr](../../aethyr/book/chapters.json) and
[Nostr](../../nostr/book/chapters.json) retain each named teaching scope,
qualified targets and reviewed component adoption. See
[completion milestones](completion.md) for the remaining integration, quality,
target and application gates. The optional original-model route below retains
its older, separately dated measurements and is outside this four-route catalog.
Those original-model command observations are from 2026-10-31 on an Apple
M5 Max with 128 GB memory and macOS 26.4, using `coax0 0.45.0`. They were not
rerun for this catalog and are not current performance guarantees.

## Write current Coax and cross a local C boundary

Route: `coax-current-ffi`. It runs the existing
[hello](../../coax/book/examples/ch01_hello_name.coax),
[explicit error results](../../coax/book/examples/ch07_ports3.coax) and
[shared source identity](../../coax/book/examples/ch10_sources.coax) examples.
Their same-stem `.out` files are the exact expected transcripts. The first
prints `Hello, Coax!`; the error example accepts ports 8080 and 443 while
reporting `not_a_digit`, `empty` and `out_of_range`; the loaded modules print 42.

```shell
python3 scripts/book_routes.py --route coax-current-ffi \
  --coax /absolute/path/to/coax --receipt /tmp/coax-current-ffi.json
```

The final step runs the existing
[C workflow](../../coax/book/examples/ch24_workflow/run.sh): emitted Coax reads
and writes a protobuf packet through a C host and a temporary local file. Its
[fixture](../../coax/book/examples/ch24_workflow/run.out) reports revision 2,
result 42, a stale completion ignored, and failure/timeout paths checked, once
for each C build. The route disables optional protoc comparison, mutation and
sanitizers so those separate gates cannot silently change its transcript. It
requires neither Swift nor a network service. Temporary build/file effects do
not change an app or publish anything.

Read Coax chapters 1–4, 7, 10, 20 and 24 for this route. Chapters 5–9, 11,
13, 16, 18 and chapter 29's runnable affine section explain related supported
forms and their explicit limits. This selected route does not exercise every
generic constraint, memory boundary, target or compiler feature. General
fallible foreign results, asynchronous ownership and remaining language work
keep their own [completion gates](completion.md#coax-a-dependable-language-and-supported-consumers).

## Replay the Aethyr engine core

Route: `aethyr-core-replay`. The three existing examples run a fixed-step clock
at four frame rates, consume a queued press exactly once, then record and
replay a small walk. They use the CPU and terminal only.

```shell
python3 scripts/book_routes.py --route aethyr-core-replay \
  --coax /absolute/path/to/coax --receipt /tmp/aethyr-core-replay.json
```

Exact expected outputs are
[frame-rates](../../aethyr/fixtures/frame-rates.out),
[one-press](../../aethyr/fixtures/one-press.out) and
[replay-a-walk](../../aethyr/fixtures/replay-a-walk.out). The replay prints
`662099296` for both recorded and replayed session digests, and reports the
planted fault's first divergence at step 150. The clock fixture also shows
an eight-step stall cap; the input fixture shows one press consumed over
frames containing zero, two and one simulation steps.

Read Coax chapters 11 and 16, then Aethyr chapters 1–5. Continue through
Aethyr 6–10 for entities, queries, ground/collision, walking and camera
composition. Existing consumers such as
[Grid walking](../../verse/gridwalk.coax) adopt particular shared components;
these terminal examples do not migrate a whole realm.

This route does not start the browser or qualify a rendered/playable Grid,
general rigid-body solver, live audio or Forge artifact. Existing native/Wasm
replay parity has a separate harness and retained evidence, including the
[chapter 9 record](../../aethyr/tests/book-chapter9-qualification.json).
Follow the [playable-engine milestones](completion.md#aethyr-a-complete-playable-engine-route)
and the [web app guide](../../apps/openagents-web/README.md) for those distinct
integrations; a CPU replay pass does not promote them.

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

## Use public models and a trainer

Route: `psionic-public-models`. Its first example actually trains a tiny
76-parameter public module for 400 SGD steps with frozen normalization and
deterministic dropout, then resumes from step 200. It compares all 400 losses
and every saved float with its reference loop, and refuses a mismatched
configuration and stale checkpoint pointer. Its
[exact transcript](../../psionic/fixtures/one-trainer-module.out) records both
the successful resume and those refusals.

```shell
python3 scripts/book_routes.py --route psionic-public-models \
  --coax /absolute/path/to/coax --receipt /tmp/psionic-public-models.json
```

The next two steps construct and execute the public
[MLP](../../psionic/examples/mnist-classifier.coax) and
[CNN](../../psionic/examples/cnn-classifier.coax) models. Their fixtures
([MLP](../../psionic/fixtures/mnist-classifier.out),
[CNN](../../psionic/fixtures/cnn-classifier.out)) show 3,322 and 2,910 parameters,
a synthetic zero-model loss of 1, a bias derivative of -2, and numerical
refusal preserving the published gradient. These two steps use no dataset and
perform no MNIST learning experiment.

Read Psionic chapters 3, 38, 106, 117–118, 122 and 124. The ordinary model APIs
connect views, explicit tensor IR, first-order autodiff, named modules,
training state and checkpointing. Retained
[MLP](../verification/psionic-mnist-framework-2026-11-01.json) and
[CNN](../verification/psionic-cnn-framework-2026-11-01.json) records separately
cover their frozen original-data gradients, training quality, checkpoint replay
and export. The route's exact output does not requalify those experiments on
a new compiler or host, and does not retrain them.

This CNN profile uses CPU f32 NCHW/OIHW valid stride-one convolution and formal
first-order f64 tape derivatives. GPU convolution, compiled convolution
backward and higher-order convolution transforms refuse; pooling, additional
options, residual models and broader architecture acceptance remain work.
Tests and teaching examples do not count as product adoption. Native experiment
consumers and the broader [framework acceptance projects](../../psionic/book/PLAN.md#a-framework-not-only-engines)
keep their own workload, target and quality gates.

## Sign and verify an event, then replay a client

Route: `nostr-event-client`. The first step signs a whole event with a published
BIP-340 test key, reads it back, verifies the ID/signature and demonstrates
altered-field refusals. The second drives the client connection state machine
through scripted handshakes, timeout/backoff, reconnects, verified events,
duplicate/stale reports, acknowledgments and shutdown.

```shell
python3 scripts/book_routes.py --route nostr-event-client \
  --coax /absolute/path/to/coax --receipt /tmp/nostr-event-client.json
```

Exact expected transcripts are [events](../../nostr/fixtures/events.out) and
[relay-connection](../../nostr/fixtures/relay-connection.out). The event step
retains its declared 250,000,000-step evaluator budget and 240-second command
timeout; the connection step has a 120-second timeout. No socket, relay,
Keychain, user key or wallet is opened. The connection's bytes and clock are
scripted in memory, and signing material is a committed public test vector.

Read Coax chapters 1–4, 11 and 18, then Bitcoin chapters 1–4, 8, 13–16, 18, 23
and 24, then Nostr chapters 17, 21–24 and 29. The cryptographic and client components have actual consumers, including
[the Pylon](../../psionic/lib/pylon/run.coax), whose real transports and
admission paths retain their own evidence. This offline route does not qualify
native TLS, browser secret handling, durable storage, live interoperability or
production relay operation. Chapters 103–104 and the
[Nostr completion milestones](completion.md#nostr-finish-real-consumers-and-operational-qualification)
keep migration, cutover and treasury signing/recovery separate.
